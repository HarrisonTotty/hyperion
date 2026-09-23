//! The hooks through which features change the gas: holes and clouds (plan 07, Design note 16).
//!
//! Plan 09's superbubbles are holes in the gas field and its molecular clouds and embedded regions
//! are clouds that add their own gas and dust. This plan defines the type they produce,
//! [`GasModifier`], and integrates it; until plan 09 the only source is [`NoModifiers`], and with an
//! empty list every result is bit-identical to one computed without modifiers.
//!
//! The point rule: a **hole** replaces the field inside its sphere by its interior density, which
//! carries no dust; where holes overlap the lowest interior density wins. A **cloud** is a Plummer
//! ball, `n_c (1 + r² ÷ a²)^(−5/2)`, added on top of whatever the field or a hole leaves, with its
//! own dust-to-gas ratio. The Plummer form is chosen because its column along a line is algebraic
//! (the extinction integral's Design note 16), so clouds cost the integral no steps and no
//! transcendental function; the interface sketch's Gaussian cloud is replaced by it, and plan 09 as
//! it now reads uses the Plummer form.

use crate::coords::GalacticPosition;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{HydrogenPerCm3, LightYears};

/// A change to the gas field that a feature makes (Design note 16).
///
/// Lengths are light-years and densities hydrogen nuclei per cm³. A hole's radius and a cloud's
/// core radius must be positive and finite and every density finite and not negative: a modifier
/// that breaks this is a bug in its source, which the debug build asserts; in a release build a hole
/// with no radius contains nothing and a cloud with no core adds nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GasModifier {
    /// A sphere inside which the field is replaced by `interior`, with no dust: a superbubble.
    Hole {
        /// The sphere's centre.
        centre: GalacticPosition,
        /// The sphere's radius.
        radius: LightYears,
        /// The density inside it.
        interior: HydrogenPerCm3,
    },
    /// A Plummer ball `n_c (1 + r² ÷ a²)^(−5/2)` of gas added on top of the field, with its own
    /// dust: a molecular cloud or an embedded star-forming region.
    Cloud {
        /// The ball's centre.
        centre: GalacticPosition,
        /// Its core radius `a`, inside which the density is within a factor of 5.7 of the centre's.
        core_radius: LightYears,
        /// Its central density `n_c`.
        central_density: HydrogenPerCm3,
        /// Its dust per hydrogen nucleus in units of the solar ratio, as
        /// [`GasField::dust_per_hydrogen`](super::field::GasField::dust_per_hydrogen) gives the
        /// field's.
        dust_per_hydrogen: f64,
    },
}

impl GasModifier {
    /// Whether the modifier's lengths are positive and finite and its densities finite and not
    /// negative: what its source must guarantee.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let length = |ly: LightYears| ly.value().is_finite() && ly.value() > 0.0;
        let amount = |value: f64| value.is_finite() && value >= 0.0;
        match *self {
            Self::Hole {
                radius, interior, ..
            } => length(radius) && amount(interior.value()),
            Self::Cloud {
                core_radius,
                central_density,
                dust_per_hydrogen,
                ..
            } => {
                length(core_radius) && amount(central_density.value()) && amount(dust_per_hydrogen)
            }
        }
    }
}

/// Where the modifiers of a line of sight come from: plan 09's features, or none.
///
/// A source appends to `out` every modifier that could touch the segment from `a` to `b` and leaves
/// what `out` already held. What it appends must be a pure function of the unordered pair `{a, b}`,
/// so that the extinction from `a` to `b` is the same bits as from `b` to `a`: a hole the segment
/// misses changes nothing, but a cloud adds a little at any distance, so an extra cloud moves the
/// last bits. The order of the list is immaterial to
/// [`sightline`](super::extinction::sightline), which sums clouds in a canonical order; the point
/// rule adds them in the list's order. Plan 09's `features::gas_overlay::FeatureGas` implements it with a [`GasModifier::Hole`]
/// per superbubble and a [`GasModifier::Cloud`] per cloud and embedded region.
pub trait GasModifierSource {
    /// Appends to `out` the modifiers near the segment from `a` to `b`.
    fn modifiers_near_segment(
        &self,
        a: &GalacticPosition,
        b: &GalacticPosition,
        out: &mut Vec<GasModifier>,
    );
}

/// The source with no modifiers: the only one until plan 09's features exist.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoModifiers;

impl GasModifierSource for NoModifiers {
    fn modifiers_near_segment(
        &self,
        _a: &GalacticPosition,
        _b: &GalacticPosition,
        _out: &mut Vec<GasModifier>,
    ) {
    }
}

/// The gas at a point once `mods` are applied to what the field has there: the hydrogen density
/// `density` (cm⁻³) and the dust-bearing density `dust` (cm⁻³ of hydrogen times the dust-to-gas
/// ratio), by the point rule of the module documentation. Returns the pair in the same units.
///
/// With no modifiers the pair comes back as it went in, bit for bit.
#[must_use]
pub(crate) fn apply(
    mods: &[GasModifier],
    p: &GalacticPosition,
    density: f64,
    dust: f64,
) -> (f64, f64) {
    if mods.is_empty() {
        return (density, dust);
    }
    let mut interior: Option<f64> = None;
    for modifier in mods {
        debug_assert!(modifier.is_valid(), "an invalid gas modifier {modifier:?}");
        if let GasModifier::Hole {
            centre,
            radius,
            interior: inside,
        } = *modifier
            && distance_ly(&centre, p) < radius.value()
        {
            let inside = inside.value();
            interior = Some(interior.map_or(
                inside,
                |lowest| if inside < lowest { inside } else { lowest },
            ));
        }
    }
    let (mut density, mut dust) = match interior {
        Some(inside) => (inside, 0.0),
        None => (density, dust),
    };
    for modifier in mods {
        if let GasModifier::Cloud {
            centre,
            core_radius,
            central_density,
            dust_per_hydrogen,
        } = *modifier
        {
            let cloud = plummer(
                central_density.value(),
                core_radius.value(),
                distance_ly(&centre, p),
            );
            density += cloud;
            dust += dust_per_hydrogen * cloud;
        }
    }
    (density, dust)
}

/// The Plummer density `n_c (1 + r² ÷ a²)^(−5/2)` at distance `r` from the centre of a ball of
/// central density `central` and core radius `core` (both lengths in ly), with no transcendental
/// function: `u^(−5/2) = 1 ÷ (u² √u)`. A core that is not positive adds nothing.
#[must_use]
pub(crate) fn plummer(central: f64, core: f64, r: f64) -> f64 {
    if core.is_nan() || core <= 0.0 {
        return 0.0;
    }
    let ratio = r / core;
    let u = 1.0 + ratio * ratio;
    central / (u * u * u.sqrt())
}

/// The distance from `a` to `b`, light-years, from the exact cell-and-offset difference.
#[must_use]
pub(crate) fn distance_ly(a: &GalacticPosition, b: &GalacticPosition) -> f64 {
    a.distance_to(b).value() / METRES_PER_LIGHT_YEAR
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn at(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("inside i32")
    }

    fn hole(centre: [f64; 3], radius: f64, interior: f64) -> GasModifier {
        GasModifier::Hole {
            centre: at(centre),
            radius: LightYears::new(radius),
            interior: HydrogenPerCm3::new(interior),
        }
    }

    fn cloud(centre: [f64; 3], core: f64, central: f64, dust: f64) -> GasModifier {
        GasModifier::Cloud {
            centre: at(centre),
            core_radius: LightYears::new(core),
            central_density: HydrogenPerCm3::new(central),
            dust_per_hydrogen: dust,
        }
    }

    /// With no modifiers the gas comes back as it went in, bit for bit, and the source with none
    /// adds none.
    #[test]
    fn no_modifiers_change_nothing() {
        let p = at([26_000.0, 10.0, 5.0]);
        let (n, d) = apply(&[], &p, 0.731, 0.614);
        assert_same_bits(n, 0.731);
        assert_same_bits(d, 0.614);
        let mut out = vec![hole([0.0; 3], 1.0, 0.0)];
        NoModifiers.modifiers_near_segment(&p, &p, &mut out);
        assert_eq!(out.len(), 1);
    }

    /// Inside a hole the field is its interior density with no dust, and the lowest of
    /// overlapping holes wins; outside every hole nothing changes.
    #[test]
    fn a_hole_replaces_the_field_inside_its_sphere() {
        let p = at([26_000.0, 0.0, 0.0]);
        let one = hole([26_010.0, 0.0, 0.0], 50.0, 0.004);
        let (n, d) = apply(&[one], &p, 0.8, 0.7);
        assert_same_bits(n, 0.004);
        assert_same_bits(d, 0.0);
        let lower = hole([25_990.0, 5.0, 0.0], 30.0, 0.002);
        let (n, _) = apply(&[one, lower], &p, 0.8, 0.7);
        assert_same_bits(n, 0.002);
        let (n, _) = apply(&[lower, one], &p, 0.8, 0.7);
        assert_same_bits(n, 0.002);
        let far = hole([26_100.0, 0.0, 0.0], 50.0, 0.004);
        let (n, d) = apply(&[far], &p, 0.8, 0.7);
        assert_same_bits(n, 0.8);
        assert_same_bits(d, 0.7);
    }

    /// A cloud adds its central density at its centre, with its own dust, on top of the field or of
    /// a hole it sits in, and falls as the Plummer law says.
    #[test]
    fn a_cloud_adds_its_plummer_density() {
        let centre = [26_000.0, 3.0, -2.0];
        let c = cloud(centre, 4.0, 300.0, 1.5);
        let (n, d) = apply(&[c], &at(centre), 0.8, 0.7);
        assert!((n - 300.8).abs() < 1e-12);
        assert!((d - (0.7 + 450.0)).abs() < 1e-12);
        let (n, d) = apply(&[hole(centre, 20.0, 0.01), c], &at(centre), 0.8, 0.7);
        assert!((n - 300.01).abs() < 1e-12);
        assert!((d - 450.0).abs() < 1e-12);
        // At one core radius the density is 2^(−5/2) of the centre's.
        let edge = at([26_004.0, 3.0, -2.0]);
        let (n, _) = apply(&[c], &edge, 0.0, 0.0);
        assert!((n / (300.0 / 32.0_f64.sqrt()) - 1.0).abs() < 1e-9, "{n}");
        assert_same_bits(plummer(1.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn a_modifier_knows_whether_it_is_valid() {
        assert!(hole([0.0; 3], 10.0, 0.0).is_valid());
        assert!(!hole([0.0; 3], 0.0, 0.1).is_valid());
        assert!(!hole([0.0; 3], 10.0, -0.1).is_valid());
        assert!(cloud([0.0; 3], 1.0, 100.0, 1.0).is_valid());
        assert!(!cloud([0.0; 3], f64::NAN, 100.0, 1.0).is_valid());
        assert!(!cloud([0.0; 3], 1.0, 100.0, f64::INFINITY).is_valid());
    }
}
