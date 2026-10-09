//! The craters' shared physics: one cumulative crater density for the coarse pass and the
//! synthesis, here because the synthesis runs in this crate and plan 14 calls it from the sim
//! (plan R09, Design note 12).
//!
//! Today it holds [`CraterParams`], a body's crater contract with plan 14, which a coarse field's
//! header carries; R09.T7.a gives it its behaviour: the cumulative density from Neukum, Ivanov and
//! Hartmann's production polynomial, the saturation cap, the transition diameter and the inversion
//! of a diameter within an octave.

use hyperion_base::units::{
    KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecond,
    MetresPerSecondSquared, PerSquareKilometre,
};

/// What screens a body's small impactors before they crater the ground (Design note 12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screening {
    /// Nothing: an airless body.
    None,
    /// An atmosphere, by its column mass P ÷ g and the projectiles' density: the projectile scale
    /// d* = 1.5 (P ÷ g) ÷ `ρ_p`, about 5.2 m for Earth, 0.52 km for Venus and 8.2 cm for Mars (at a
    /// projectile density of 3,000 kg m⁻³, and 610 Pa on Mars), gives a crater cutoff of about
    /// 20 d* (Design note 12; R09.T7.a builds it).
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
    /// The target factor `k_target`, dimensionless: 1 for rock, 0.12 for an ice-rich crust
    /// (Design note 3); finite and positive.
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
    /// finite and positive, or its cutoff not finite and positive.
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
        if !screening_ok {
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
    /// note 3).
    #[must_use]
    pub const fn k_target(&self) -> f64 {
        self.parts.k_target
    }

    /// The mean impact velocity, m/s, if plan 14 gives one: the π-group diameter map is the
    /// identity without it (Design note 12, after Holsapple 1993).
    #[must_use]
    pub const fn impact_velocity(&self) -> Option<MetresPerSecond> {
        self.parts.impact_velocity
    }
}

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
