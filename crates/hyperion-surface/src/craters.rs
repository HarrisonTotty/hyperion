//! The craters' shared physics: one cumulative crater density and one family of fresh crater
//! shapes for the coarse pass and the synthesis, here because the synthesis runs in this crate and
//! plan 14 calls the density from the sim (plan R09, Design notes 12 and 13).
//!
//! - [`CraterParams`] is a body's crater contract with plan 14, which a coarse field's header
//!   carries: N(>1 km), what screens small impactors, the surface gravity, the target factor and,
//!   optionally, a mean impact velocity.
//! - [`cumulative_density`] is the body's crater density N(>D) per square kilometre: Neukum,
//!   Ivanov and Hartmann 2001's lunar production polynomial with a0 = log₁₀ N(>1 km), extended
//!   beyond its 10 m–300 km range by its end slopes, mapped to the body by π-group scaling when
//!   [`CraterParams::impact_velocity`] is given, and tapered on the differential production by
//!   the atmosphere's screening. [`production_density`] is the same before screening, and
//!   [`saturation`] Trask's equilibrium cap. [`diameter_in_octave`] inverts the density within a
//!   [`DiameterOctave`] by bisection in log D with a fixed iteration count.
//! - [`transition_diameter`] is the simple-to-complex transition, and [`crater_shape`] a fresh
//!   crater's [`CraterShape`]: its depth, rim, floor and central peak by morphological zone
//!   (`decision-r09-t2.md` item 3), and the width of its volume-balanced exterior (Design note 13).
//!
//! # Bodies the lunar laws do not describe
//!
//! Every law here is fitted on rocky bodies, mostly the Moon, and reaches other bodies through
//! three inputs alone:
//!
//! - _Gravity_ sets the transition diameter (∝ 1 ÷ g) and so every complex crater's dimensions,
//!   the regolith bowls' size scale, and, with the impact velocity, the diameter map.
//! - _The target_ enters through `k_target`, which scales the transition diameter: 1 for rock and
//!   0.12 for an ice-rich crust (Design note 12, after science-r09-craters.md item 1: the
//!   morphological transitions of nine icy bodies give 0.125, 0.06–0.26; Ganymede's 2.1 ± 0.2 km,
//!   Schenk et al. 2004; Rhea's 12 ± 2 km, Aponte-Hernández et al. 2021; Ceres's mixed crust
//!   behaves as ice). No temperature term is resolvable between 40 and 150 K. Ganymede's complex
//!   depths come out within 22% and its rims within 5% of Schenk 1991's (`decision-r09-t2.md`
//!   item 3). The diameter map takes non-porous rock's exponents for every target, and no target
//!   or projectile density; R09.T7.e adds the target's scaling class and density (ice is
//!   non-porous at 940 kg m⁻³) and the projectiles' density (science-r09-craters.md item 2).
//! - _The atmosphere_ enters through [`Screening`]: one taper for every atmosphere, fitted on
//!   Venus's craters (Design note 12), from an airless body's none to any column mass. An
//!   atmosphere far thicker than Venus's only moves the cutoff.
//! - _Time_ enters through N(>1 km), which plan 14 takes from the [`lunar_chronology`] counted
//!   from the body's own system's formation, and [`chronology_age`] inverts it for craters' ages
//!   (science-r09-craters.md item 5).
//!
//! All arithmetic goes through `hyperion_base::math`, sums run in a fixed order and every
//! quadrature and bisection has a fixed number of nodes or steps, so the server and the client's
//! WebAssembly get the same bits.

pub mod chronology;
pub mod density;
pub mod shape;

pub use chronology::{SOLAR_SYSTEM_AGE, chronology_age, lunar_chronology};
pub use density::{
    BuildDiameterOctaveError, BuildUnitUniformError, DiameterOctave, UnitUniform, crater_cutoff,
    cumulative_density, diameter_in_octave, production_density, projectile_scale, saturation,
};
pub use shape::{CraterShape, crater_shape};

use hyperion_base::units::{
    KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecond,
    MetresPerSecondSquared, PerSquareKilometre,
};

/// The Moon's mean impact velocity under P14.T48.f's model, the reference of the diameter map.
///
/// Ivanov 2001's 16.1 km/s, with which he built Mars's function (Ivanov et al. 2002, §5.4), and
/// the debiased 19–20 km/s bracket it (science-r09-craters.md item 4).
pub const LUNAR_IMPACT_VELOCITY: MetresPerSecond = MetresPerSecond::new(16_700.0);

/// The Moon's surface gravity as the plan's lunar laws use it, m s⁻²: 1.62 (Design note 12, the
/// figure its transition diameter and its regolith bowls scale from).
const MOON_GRAVITY_M_PER_S2: f64 = 1.62;

/// The Moon's simple-to-complex transition diameter, metres: 19 km (Design note 12, after Pike
/// 1980a; Krüger, Hergarten and Kenkmann 2018 put the lunar zone at 14–17 km to 24–28 km, 0.74–1.47
/// of it).
const MOON_TRANSITION_M: f64 = 19_000.0;

/// The simple-to-complex transition diameter `D_t` of a body: 19 km × (1.62 m s⁻² ÷ g) ×
/// `k_target` (Design note 12; Pike 1980a, Table 3, whose Earth–Moon pair gives g^−1.01).
///
/// It is the centre of a zone, not a line: bowls below 0.8 `D_t`, transitional craters to
/// 1.5 `D_t`, central peaks above, peak rings from about 9 `D_t` and multi-ring basins above about
/// 16 `D_t` ([`Morphology`](crate::field::Morphology)).
///
/// About 19 km on the Moon, 8.3 km on Mercury and Mars, 3.1 km on Earth and 2.6 km on Ganymede
/// (`k_target` 0.12). Susorney et al. 2016 measure 11.7 ± 1.2 km on Mercury and Robbins and Hynek
/// 2012 about 6 km on Mars, the known Mercury–Mars anomaly, which one law in g cannot follow (plan
/// R09, Risks).
#[must_use]
pub fn transition_diameter(p: &CraterParams) -> Metres {
    Metres::new(MOON_TRANSITION_M * (MOON_GRAVITY_M_PER_S2 / p.gravity().value()) * p.k_target())
}

/// What screens a body's small impactors before they crater the ground (Design note 12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screening {
    /// Nothing: an airless body.
    None,
    /// An atmosphere, by its column mass P ÷ g and the projectiles' density: the projectile scale
    /// d\* = 1.5 (P ÷ g) ÷ `ρ_p` ([`projectile_scale`]), about 5.2 m for Earth, 0.52 km for Venus
    /// and 8.5 cm for Mars at NASA's mean surface pressure of 636 Pa (at a projectile density of
    /// 3,000 kg m⁻³), gives the crater cutoff `D_c` = 20 d\* ([`crater_cutoff`]; Design note 12).
    /// A column mass of zero screens nothing, as [`Screening::None`].
    Atmosphere {
        /// The atmosphere's column mass, the surface pressure over the surface gravity, kg m⁻²:
        /// about 10,300 kg m⁻² on Earth.
        column_mass: KilogramsPerSquareMetre,
        /// The projectiles' bulk density, kg m⁻³.
        projectile_density: KilogramsPerCubicMetre,
    },
    /// A crater cutoff diameter given directly, metres.
    Cutoff {
        /// The cutoff diameter, metres.
        diameter: Metres,
    },
}

/// The parts of a [`CraterParams`], each in the unit its type states.
///
/// Plain data with public fields: [`CraterParams::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterParamsParts {
    /// The cumulative crater density above 1 km, N(>1 km), per square kilometre, with plan 14's
    /// belt scaling already in it; finite and non-negative.
    pub n_1km: PerSquareKilometre,
    /// What screens the body's small impactors.
    pub screening: Screening,
    /// The surface gravity, m s⁻²; finite and positive.
    pub gravity: MetresPerSecondSquared,
    /// The target factor `k_target`, dimensionless, which scales the transition diameter: 1 for
    /// rock, 0.12 for an ice-rich crust (Design note 12, sourced by science-r09-craters.md item 1);
    /// finite and positive.
    pub k_target: f64,
    /// The mean impact velocity, m/s, if plan 14 gives one; finite and positive.
    pub impact_velocity: Option<MetresPerSecond>,
}

/// A body's crater contract with plan 14 (Design note 3's crater contract, Design note 12): the
/// cumulative density N(>1 km), what screens small impactors, the surface gravity, the target's
/// factor and, optionally, a mean impact velocity. The surface crate derives the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterParams {
    parts: CraterParamsParts,
}

/// Why [`CraterParams::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildCraterParamsError {
    /// N(>1 km) is not finite and non-negative.
    Density(f64),
    /// The gravity is not finite and positive.
    Gravity(f64),
    /// The target factor is not finite and positive.
    TargetFactor(f64),
    /// The impact velocity is not finite and positive.
    ImpactVelocity(f64),
    /// The screening's column mass is not finite and non-negative, its projectile density not
    /// finite and positive, or its cutoff not finite and positive; or the crater cutoff it gives
    /// is out of the density's range, under about 10⁻³²⁰ m or beyond about 10³⁰⁵ m.
    Screening(Screening),
}

impl std::fmt::Display for BuildCraterParamsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Density(n) => write!(
                f,
                "crater density N(>1 km) {n} km⁻² is not finite and non-negative"
            ),
            Self::Gravity(g) => write!(f, "gravity {g} m s⁻² is not finite and positive"),
            Self::TargetFactor(k) => write!(f, "target factor {k} is not finite and positive"),
            Self::ImpactVelocity(v) => {
                write!(f, "impact velocity {v} m/s is not finite and positive")
            }
            Self::Screening(s) => write!(f, "screening {s:?} is out of range"),
        }
    }
}

impl std::error::Error for BuildCraterParamsError {}

/// Whether `x` is finite and positive.
#[must_use]
fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

impl CraterParams {
    /// The contract of `parts`.
    ///
    /// # Errors
    ///
    /// [`BuildCraterParamsError`] if the density is not finite and non-negative, the gravity, the
    /// target factor or the velocity is not finite and positive, or the screening's parts are out
    /// of range.
    pub fn new(parts: CraterParamsParts) -> Result<Self, BuildCraterParamsError> {
        let n = parts.n_1km.value();
        if !(n.is_finite() && n >= 0.0) {
            return Err(BuildCraterParamsError::Density(n));
        }
        if !positive(parts.gravity.value()) {
            return Err(BuildCraterParamsError::Gravity(parts.gravity.value()));
        }
        if !positive(parts.k_target) {
            return Err(BuildCraterParamsError::TargetFactor(parts.k_target));
        }
        if let Some(v) = parts.impact_velocity
            && !positive(v.value())
        {
            return Err(BuildCraterParamsError::ImpactVelocity(v.value()));
        }
        let screening_ok = match parts.screening {
            Screening::None => true,
            Screening::Atmosphere {
                column_mass,
                projectile_density,
            } => {
                let m = column_mass.value();
                m.is_finite() && m >= 0.0 && positive(projectile_density.value())
            }
            Screening::Cutoff { diameter } => positive(diameter.value()),
        };
        if !(screening_ok && density::screening_in_range(parts.screening)) {
            return Err(BuildCraterParamsError::Screening(parts.screening));
        }
        Ok(Self { parts })
    }

    /// The validated parts.
    #[must_use]
    pub const fn parts(&self) -> &CraterParamsParts {
        &self.parts
    }

    /// The cumulative crater density above 1 km, N(>1 km), per square kilometre: the production
    /// polynomial's a0 as log₁₀ of it (Design note 12), with plan 14's belt scaling already in it.
    #[must_use]
    pub const fn n_1km(&self) -> PerSquareKilometre {
        self.parts.n_1km
    }

    /// What screens the body's small impactors.
    #[must_use]
    pub const fn screening(&self) -> Screening {
        self.parts.screening
    }

    /// The surface gravity, m s⁻².
    #[must_use]
    pub const fn gravity(&self) -> MetresPerSecondSquared {
        self.parts.gravity
    }

    /// The target factor `k_target`, dimensionless: 1 for rock, 0.12 for an ice-rich crust (Design
    /// note 12, sourced by science-r09-craters.md item 1), which scales the
    /// [`transition_diameter`].
    #[must_use]
    pub const fn k_target(&self) -> f64 {
        self.parts.k_target
    }

    /// The mean impact velocity, m/s, if plan 14 gives one: it switches on the π-group diameter map
    /// from the Moon's craters to the body's ([`cumulative_density`]), which is the identity
    /// without it, and at the lunar reference ([`LUNAR_IMPACT_VELOCITY`], g 1.62, `D_t` 19 km;
    /// Design note 12).
    #[must_use]
    pub const fn impact_velocity(&self) -> Option<MetresPerSecond> {
        self.parts.impact_velocity
    }
}

#[cfg(test)]
mod golden;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn lunar() -> CraterParamsParts {
        CraterParamsParts {
            n_1km: PerSquareKilometre::new(0.018),
            screening: Screening::None,
            gravity: MetresPerSecondSquared::new(1.62),
            k_target: 1.0,
            impact_velocity: None,
        }
    }

    #[test]
    fn crater_params_refuse_parts_out_of_range() {
        let ok = CraterParams::new(lunar()).unwrap();
        assert_eq!(ok.parts(), &lunar());
        assert_eq!(ok.n_1km(), PerSquareKilometre::new(0.018));
        assert_eq!(ok.screening(), Screening::None);
        let refuse = |edit: fn(&mut CraterParamsParts)| {
            let mut parts = lunar();
            edit(&mut parts);
            CraterParams::new(parts).unwrap_err()
        };
        assert_eq!(
            refuse(|p| p.n_1km = PerSquareKilometre::new(-1.0)),
            BuildCraterParamsError::Density(-1.0)
        );
        assert_eq!(
            refuse(|p| p.gravity = MetresPerSecondSquared::ZERO),
            BuildCraterParamsError::Gravity(0.0)
        );
        assert_eq!(
            refuse(|p| p.k_target = 0.0),
            BuildCraterParamsError::TargetFactor(0.0)
        );
        assert_eq!(
            refuse(|p| p.impact_velocity = Some(MetresPerSecond::new(-1.0))),
            BuildCraterParamsError::ImpactVelocity(-1.0)
        );
        let thin = Screening::Atmosphere {
            column_mass: KilogramsPerSquareMetre::new(164.0),
            projectile_density: KilogramsPerCubicMetre::ZERO,
        };
        let mut parts = lunar();
        parts.screening = thin;
        assert_eq!(
            CraterParams::new(parts),
            Err(BuildCraterParamsError::Screening(thin))
        );
    }
}
